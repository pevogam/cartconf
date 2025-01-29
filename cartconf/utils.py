"""
Utils module.
"""

import re

from .constants import reserved_keys


def convert_data_size(size, default_sufix='B'):
    """
    Convert data size from human readable units to an int of arbitrary size.

    :param size: Human readable data size representation (string).
    :param default_sufix: Default sufix used to represent data.
    :return: Int with data size in the appropriate order of magnitude.
    """
    orders = {'B': 1,
              'K': 1024,
              'M': 1024 * 1024,
              'G': 1024 * 1024 * 1024,
              'T': 1024 * 1024 * 1024 * 1024,
              }

    order = re.findall("([BbKkMmGgTt])", size[-1])
    if not order:
        size += default_sufix
        order = [default_sufix]

    return int(float(size[0:-1]) * orders[order[0].upper()])


def compare_string(str1, str2):
    """
    Compare two int string and return -1, 0, 1.
    It can compare two memory value even in sufix

    :param str1: The first string
    :param str2: The second string

    :Return: Return -1, when str1<  str2
                     0, when str1 = str2
                     1, when str1>  str2
    """
    order1 = re.findall("([BbKkMmGgTt])", str1)
    order2 = re.findall("([BbKkMmGgTt])", str2)
    if order1 or order2:
        value1 = convert_data_size(str1, "M")
        value2 = convert_data_size(str2, "M")
    else:
        value1 = int(str1)
        value2 = int(str2)
    if value1 < value2:
        return -1
    elif value1 == value2:
        return 0
    else:
        return 1


def postfix_parse(dic):
    tmp_dict = {}
    for key in dic:
        # Bypass the case that use tuple as key value
        if isinstance(key, tuple):
            continue
        if key.endswith("_max"):
            tmp_key = key.split("_max")[0]
            if (tmp_key not in dic or
                    compare_string(dic[tmp_key], dic[key]) > 0):
                tmp_dict[tmp_key] = dic[key]
        elif key.endswith("_min"):
            tmp_key = key.split("_min")[0]
            if (tmp_key not in dic or
                    compare_string(dic[tmp_key], dic[key]) < 0):
                tmp_dict[tmp_key] = dic[key]
        elif key.endswith("_fixed"):
            tmp_key = key.split("_fixed")[0]
            tmp_dict[tmp_key] = dic[key]
    for key in tmp_dict:
        dic[key] = tmp_dict[key]


def drop_suffixes(d, skipdups=True):
    """
    Merge suffixes for same var, or drop off unnecessary suffixes

    This step returns a copy of a suffix flattened dictionary.
    """
    # dictionary `d_flat' is going to be the mutated copy of `d`
    d_flat = d.copy()
    for key in d:
        if key in reserved_keys:
            continue

        if not isinstance(key, tuple):
            continue

        if skipdups:
            # Drop vars with suffixes matches general var val
            # Example: if a_x == 1, and a == 1. Drop: a_x, leave a
            gen_var_name = key[0]
            if gen_var_name in d and d[gen_var_name] == d[key]:
                # Drop gen_var_name, use general key with same value
                d_flat.pop(key)
                continue

            can_drop_all_suffixes_for_this_key = True
            for k in d:
                gen_name = k[0] if isinstance(k, tuple) else k
                if gen_var_name == gen_name:
                    if d[key] != d[k]:
                        can_drop_all_suffixes_for_this_key = False
                        break

        if skipdups and can_drop_all_suffixes_for_this_key:
            new_key = key[0]
        else:
            # merge suffixes, preserve reverse order of suffixes
            new_key = key[:1] + key[1:][::-1]
            new_key = ''.join((map(str, new_key)))
        d_flat[new_key] = d_flat.pop(key)

    return d_flat
